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
        let _ = (attr_id, out_id);
        g
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// TYPED PIN VALUES
// ═══════════════════════════════════════════════════════════════════════════════

/// Strongly-typed runtime value flowing through a pin.
#[derive(Debug, Clone, PartialEq)]
pub enum PinValue {
    Float(f32),
    Int(i32),
    Bool(bool),
    Vec2([f32; 2]),
    Vec3([f32; 3]),
    Vec4([f32; 4]),
    Color([f32; 4]),
}

impl PinValue {
    pub fn as_float(&self) -> f32 {
        match self {
            PinValue::Float(v)  => *v,
            PinValue::Int(v)    => *v as f32,
            PinValue::Bool(v)   => if *v { 1.0 } else { 0.0 },
            PinValue::Vec2(v)   => v[0],
            PinValue::Vec3(v)   => v[0],
            PinValue::Vec4(v)   => v[0],
            PinValue::Color(v)  => v[0],
        }
    }

    pub fn as_vec2(&self) -> [f32; 2] {
        match self {
            PinValue::Vec2(v)  => *v,
            PinValue::Vec3(v)  => [v[0], v[1]],
            PinValue::Vec4(v)  => [v[0], v[1]],
            PinValue::Float(v) => [*v, *v],
            _ => [0.0; 2],
        }
    }

    pub fn as_vec3(&self) -> [f32; 3] {
        match self {
            PinValue::Vec3(v)  => *v,
            PinValue::Vec4(v)  => [v[0], v[1], v[2]],
            PinValue::Vec2(v)  => [v[0], v[1], 0.0],
            PinValue::Float(v) => [*v, *v, *v],
            PinValue::Color(v) => [v[0], v[1], v[2]],
            _ => [0.0; 3],
        }
    }

    pub fn as_vec4(&self) -> [f32; 4] {
        match self {
            PinValue::Vec4(v)  => *v,
            PinValue::Color(v) => *v,
            PinValue::Vec3(v)  => [v[0], v[1], v[2], 1.0],
            PinValue::Vec2(v)  => [v[0], v[1], 0.0, 1.0],
            PinValue::Float(v) => [*v, *v, *v, 1.0],
            _ => [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn as_int(&self) -> i32 {
        match self {
            PinValue::Int(v)   => *v,
            PinValue::Float(v) => *v as i32,
            PinValue::Bool(v)  => if *v { 1 } else { 0 },
            _ => 0,
        }
    }

    pub fn as_bool(&self) -> bool {
        match self {
            PinValue::Bool(v)  => *v,
            PinValue::Float(v) => *v != 0.0,
            PinValue::Int(v)   => *v != 0,
            _ => false,
        }
    }

    pub fn pin_type(&self) -> PinType {
        match self {
            PinValue::Float(_) => PinType::Float,
            PinValue::Int(_)   => PinType::Int,
            PinValue::Bool(_)  => PinType::Bool,
            PinValue::Vec2(_)  => PinType::Vec2,
            PinValue::Vec3(_)  => PinType::Vec3,
            PinValue::Vec4(_)  => PinType::Vec4,
            PinValue::Color(_) => PinType::Color,
        }
    }

    pub fn default_for(pt: PinType) -> PinValue {
        match pt {
            PinType::Float => PinValue::Float(0.0),
            PinType::Int   => PinValue::Int(0),
            PinType::Bool  => PinValue::Bool(false),
            PinType::Vec2  => PinValue::Vec2([0.0; 2]),
            PinType::Vec3  => PinValue::Vec3([0.0; 3]),
            PinType::Vec4  => PinValue::Vec4([0.0; 4]),
            PinType::Color => PinValue::Color([0.0, 0.0, 0.0, 1.0]),
        }
    }

    pub fn coerce_to(&self, target: PinType) -> Option<PinValue> {
        if self.pin_type() == target { return Some(self.clone()); }
        match target {
            PinType::Float => Some(PinValue::Float(self.as_float())),
            PinType::Int   => Some(PinValue::Int(self.as_int())),
            PinType::Bool  => Some(PinValue::Bool(self.as_bool())),
            PinType::Vec2  => Some(PinValue::Vec2(self.as_vec2())),
            PinType::Vec3  => Some(PinValue::Vec3(self.as_vec3())),
            PinType::Vec4  => Some(PinValue::Vec4(self.as_vec4())),
            PinType::Color => Some(PinValue::Color(self.as_vec4())),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// EVALUATION CONTEXT AND ERRORS
// ═══════════════════════════════════════════════════════════════════════════════

pub struct NodeEvalContext {
    pub time:        f32,
    pub position:    [f32; 3],
    pub seed:        u32,
    pub custom_vars: HashMap<String, f32>,
}

impl Default for NodeEvalContext {
    fn default() -> Self {
        Self { time: 0.0, position: [0.0; 3], seed: 0, custom_vars: HashMap::new() }
    }
}

impl NodeEvalContext {
    pub fn new(time: f32) -> Self { Self { time, ..Default::default() } }

    pub fn with_position(mut self, p: [f32; 3]) -> Self { self.position = p; self }

    pub fn with_seed(mut self, seed: u32) -> Self { self.seed = seed; self }

    pub fn set_var(&mut self, key: &str, val: f32) {
        self.custom_vars.insert(key.to_string(), val);
    }
}

#[derive(Debug, Clone)]
pub enum EvalError {
    CycleDetected,
    NodeNotFound(u32),
    TypeMismatch(PinType, PinType),
}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EvalError::CycleDetected        => write!(f, "Cycle detected"),
            EvalError::NodeNotFound(id)     => write!(f, "Node {} not found", id),
            EvalError::TypeMismatch(a, b)   => write!(f, "Type mismatch {:?} vs {:?}", a, b),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// NOISE PRIMITIVES
// ═══════════════════════════════════════════════════════════════════════════════

fn ng_hash_u32(mut x: u32) -> u32 {
    x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
    x = ((x >> 16) ^ x).wrapping_mul(0x45d9f3b);
    x ^ (x >> 16)
}

fn ng_hash_f32(x: i32, y: i32, z: i32, seed: u32) -> f32 {
    let h = ng_hash_u32(
        (x as u32).wrapping_add(374761393)
            .wrapping_mul(1111111111)
            ^ (y as u32).wrapping_add(668265263)
            .wrapping_mul(2246822519)
            ^ (z as u32).wrapping_add(seed)
            .wrapping_mul(3266489917),
    );
    (h as f32 / u32::MAX as f32) * 2.0 - 1.0
}

fn ng_smoothstep(t: f32) -> f32 { t * t * (3.0 - 2.0 * t) }

fn ng_value_noise(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let tx = ng_smoothstep(x - xi as f32);
    let ty = ng_smoothstep(y - yi as f32);
    let tz = ng_smoothstep(z - zi as f32);
    let c000 = ng_hash_f32(xi,   yi,   zi,   seed);
    let c100 = ng_hash_f32(xi+1, yi,   zi,   seed);
    let c010 = ng_hash_f32(xi,   yi+1, zi,   seed);
    let c110 = ng_hash_f32(xi+1, yi+1, zi,   seed);
    let c001 = ng_hash_f32(xi,   yi,   zi+1, seed);
    let c101 = ng_hash_f32(xi+1, yi,   zi+1, seed);
    let c011 = ng_hash_f32(xi,   yi+1, zi+1, seed);
    let c111 = ng_hash_f32(xi+1, yi+1, zi+1, seed);
    let x00 = c000 + tx * (c100 - c000);
    let x10 = c010 + tx * (c110 - c010);
    let x01 = c001 + tx * (c101 - c001);
    let x11 = c011 + tx * (c111 - c011);
    let y0  = x00 + ty * (x10 - x00);
    let y1  = x01 + ty * (x11 - x01);
    y0 + tz * (y1 - y0)
}

fn ng_fbm(x: f32, y: f32, z: f32, octaves: i32, lacunarity: f32, gain: f32, seed: u32) -> f32 {
    let mut value = 0.0f32;
    let mut amp   = 0.5f32;
    let mut freq  = 1.0f32;
    for _ in 0..octaves.max(1).min(8) {
        value += amp * ng_value_noise(x * freq, y * freq, z * freq, seed);
        amp  *= gain;
        freq *= lacunarity;
    }
    value
}

fn ng_worley(x: f32, y: f32, z: f32, seed: u32) -> f32 {
    let xi = x.floor() as i32;
    let yi = y.floor() as i32;
    let zi = z.floor() as i32;
    let mut min_d = f32::MAX;
    for dz in -1i32..=1 {
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let cx = xi + dx; let cy = yi + dy; let cz = zi + dz;
                let px = cx as f32 + (ng_hash_f32(cx, cy, cz, seed) * 0.5 + 0.5);
                let py = cy as f32 + (ng_hash_f32(cx+1000, cy, cz, seed) * 0.5 + 0.5);
                let pz = cz as f32 + (ng_hash_f32(cx, cy+1000, cz, seed) * 0.5 + 0.5);
                let d = ((px-x).powi(2)+(py-y).powi(2)+(pz-z).powi(2)).sqrt();
                if d < min_d { min_d = d; }
            }
        }
    }
    (min_d - 0.5) * 2.0
}

// ═══════════════════════════════════════════════════════════════════════════════
// VECTOR HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

fn v3_add(a: [f32;3], b: [f32;3]) -> [f32;3] { [a[0]+b[0], a[1]+b[1], a[2]+b[2]] }
fn v3_sub(a: [f32;3], b: [f32;3]) -> [f32;3] { [a[0]-b[0], a[1]-b[1], a[2]-b[2]] }
fn v3_scale(a: [f32;3], s: f32)   -> [f32;3] { [a[0]*s, a[1]*s, a[2]*s] }
fn v3_dot(a: [f32;3], b: [f32;3]) -> f32 { a[0]*b[0] + a[1]*b[1] + a[2]*b[2] }
fn v3_len(a: [f32;3]) -> f32 { v3_dot(a, a).sqrt() }
fn v3_norm(a: [f32;3]) -> [f32;3] {
    let l = v3_len(a);
    if l < 1e-7 { return [0.0; 3]; }
    [a[0]/l, a[1]/l, a[2]/l]
}
fn v3_cross(a: [f32;3], b: [f32;3]) -> [f32;3] {
    [a[1]*b[2]-a[2]*b[1], a[2]*b[0]-a[0]*b[2], a[0]*b[1]-a[1]*b[0]]
}
fn v3_reflect(v: [f32;3], n: [f32;3]) -> [f32;3] {
    let d = v3_dot(v, n) * 2.0;
    v3_sub(v, v3_scale(n, d))
}
fn ng_lerp(a: f32, b: f32, t: f32) -> f32 { a + (b - a) * t }
fn ng_smoothstep_range(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0 + 1e-10)).clamp(0.0, 1.0);
    ng_smoothstep(t)
}

fn ng_hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 3] {
    if s < 1e-6 { return [v, v, v]; }
    let h6 = (h * 6.0).rem_euclid(6.0);
    let i = h6 as i32;
    let f = h6 - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    match i { 0=>[v,t,p], 1=>[q,v,p], 2=>[p,v,t], 3=>[p,q,v], 4=>[t,p,v], _=>[v,p,q] }
}

fn ng_rgb_to_hsv(r: f32, g: f32, b: f32) -> [f32; 3] {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;
    let v = max;
    let s = if max < 1e-6 { 0.0 } else { delta / max };
    let h = if delta < 1e-6 { 0.0 }
            else if max == r { ((g-b)/delta).rem_euclid(6.0) / 6.0 }
            else if max == g { ((b-r)/delta + 2.0) / 6.0 }
            else             { ((r-g)/delta + 4.0) / 6.0 };
    [h, s, v]
}

// ═══════════════════════════════════════════════════════════════════════════════
// NODE EVALUATION ENGINE
// ═══════════════════════════════════════════════════════════════════════════════

/// Evaluate a single node given resolved input values.
pub fn evaluate_node(node: &GraphNode, inputs: &[PinValue], ctx: &NodeEvalContext) -> Vec<PinValue> {
    let fi = |idx: usize, def: f32| -> f32 { inputs.get(idx).map(|v| v.as_float()).unwrap_or(def) };
    let v3i = |idx: usize| -> [f32;3] { inputs.get(idx).map(|v| v.as_vec3()).unwrap_or([0.0;3]) };
    let ii  = |idx: usize, def: i32| -> i32 { inputs.get(idx).map(|v| v.as_int()).unwrap_or(def) };

    match node.name.as_str() {
        // Sources
        "Constant" => vec![PinValue::Float(node.preview_value.unwrap_or(0.0))],
        "Time"     => vec![PinValue::Float(ctx.time)],
        "Position" => {
            let [x,y,z] = ctx.position;
            vec![PinValue::Float(x), PinValue::Float(y), PinValue::Float(z), PinValue::Vec3([x,y,z])]
        }
        "Random" => {
            let h = ng_hash_u32(ctx.seed.wrapping_add(node.id.wrapping_mul(2654435761)));
            vec![PinValue::Float(h as f32 / u32::MAX as f32)]
        }
        "MousePos" => {
            let mx = ctx.custom_vars.get("mouse_x").copied().unwrap_or(0.0);
            let my = ctx.custom_vars.get("mouse_y").copied().unwrap_or(0.0);
            vec![PinValue::Vec2([mx, my])]
        }
        "Slider" => vec![PinValue::Float(node.preview_value.unwrap_or(0.5))],

        // Math
        "Add"        => vec![PinValue::Float(fi(0,0.0) + fi(1,0.0))],
        "Subtract"   => vec![PinValue::Float(fi(0,0.0) - fi(1,0.0))],
        "Multiply"   => vec![PinValue::Float(fi(0,0.0) * fi(1,1.0))],
        "Divide"     => { let b=fi(1,1.0); vec![PinValue::Float(if b.abs()<1e-10 {0.0} else {fi(0,0.0)/b})] }
        "Pow"        => vec![PinValue::Float(fi(0,0.0).powf(fi(1,2.0)))],
        "Sqrt"       => vec![PinValue::Float(fi(0,0.0).max(0.0).sqrt())],
        "Abs"        => vec![PinValue::Float(fi(0,0.0).abs())],
        "Sin"        => vec![PinValue::Float(fi(0,0.0).sin())],
        "Cos"        => vec![PinValue::Float(fi(0,0.0).cos())],
        "Tan"        => vec![PinValue::Float(fi(0,0.0).tan())],
        "Atan2"      => vec![PinValue::Float(fi(0,0.0).atan2(fi(1,1.0)))],
        "Floor"      => vec![PinValue::Float(fi(0,0.0).floor())],
        "Ceil"       => vec![PinValue::Float(fi(0,0.0).ceil())],
        "Fract"      => vec![PinValue::Float(fi(0,0.0).fract())],
        "Sign"       => vec![PinValue::Float(fi(0,0.0).signum())],
        "Min"        => vec![PinValue::Float(fi(0,0.0).min(fi(1,0.0)))],
        "Max"        => vec![PinValue::Float(fi(0,0.0).max(fi(1,0.0)))],
        "Log"        => vec![PinValue::Float(fi(0,1.0).max(1e-10).ln())],
        "Exp"        => vec![PinValue::Float(fi(0,0.0).exp())],
        "Mod"        => { let m=fi(1,1.0); vec![PinValue::Float(fi(0,0.0).rem_euclid(m))] }
        "Step"       => vec![PinValue::Float(if fi(1,0.0)>=fi(0,0.5) {1.0} else {0.0})],
        "Clamp"      => vec![PinValue::Float(fi(0,0.0).clamp(fi(1,0.0), fi(2,1.0)))],
        "Lerp"       => vec![PinValue::Float(ng_lerp(fi(0,0.0), fi(1,1.0), fi(2,0.5)))],
        "Smoothstep" => vec![PinValue::Float(ng_smoothstep_range(fi(0,0.0), fi(1,1.0), fi(2,0.5)))],

        // Vector
        "MakeVec2"  => vec![PinValue::Vec2([fi(0,0.0), fi(1,0.0)])],
        "MakeVec3"  => vec![PinValue::Vec3([fi(0,0.0), fi(1,0.0), fi(2,0.0)])],
        "MakeVec4"  => vec![PinValue::Vec4([fi(0,0.0), fi(1,0.0), fi(2,0.0), fi(3,1.0)])],
        "SplitVec2" => { let v=inputs.get(0).map(|v|v.as_vec2()).unwrap_or([0.0;2]); vec![PinValue::Float(v[0]),PinValue::Float(v[1])] }
        "SplitVec3" => { let v=v3i(0); vec![PinValue::Float(v[0]),PinValue::Float(v[1]),PinValue::Float(v[2])] }
        "SplitVec4" => { let v=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]); vec![PinValue::Float(v[0]),PinValue::Float(v[1]),PinValue::Float(v[2]),PinValue::Float(v[3])] }
        "Dot"       => vec![PinValue::Float(v3_dot(v3i(0), v3i(1)))],
        "Cross"     => vec![PinValue::Vec3(v3_cross(v3i(0), v3i(1)))],
        "Normalize" => vec![PinValue::Vec3(v3_norm(v3i(0)))],
        "Length"    => vec![PinValue::Float(v3_len(v3i(0)))],
        "Reflect"   => vec![PinValue::Vec3(v3_reflect(v3i(0), v3i(1)))],
        "Distance"  => vec![PinValue::Float(v3_len(v3_sub(v3i(0), v3i(1))))],
        "VecAdd"    => vec![PinValue::Vec3(v3_add(v3i(0), v3i(1)))],
        "VecSub"    => vec![PinValue::Vec3(v3_sub(v3i(0), v3i(1)))],
        "VecScale"  => vec![PinValue::Vec3(v3_scale(v3i(0), fi(1,1.0)))],
        "LerpVec3"  => {
            let a=v3i(0); let b=v3i(1); let t=fi(2,0.5);
            vec![PinValue::Vec3([ng_lerp(a[0],b[0],t),ng_lerp(a[1],b[1],t),ng_lerp(a[2],b[2],t)])]
        }
        "Rotate2D"  => {
            let v=inputs.get(0).map(|v|v.as_vec2()).unwrap_or([1.0,0.0]);
            let a=fi(1,0.0); let (s,c)=(a.sin(),a.cos());
            vec![PinValue::Vec2([c*v[0]-s*v[1], s*v[0]+c*v[1]])]
        }

        // Noise
        "Perlin" | "ValueNoise" => {
            let pos=v3i(0); let freq=fi(1,1.0);
            vec![PinValue::Float(ng_value_noise(pos[0]*freq, pos[1]*freq, pos[2]*freq, ctx.seed))]
        }
        "Simplex" => {
            let pos=v3i(0); let freq=fi(1,1.0);
            vec![PinValue::Float(ng_value_noise(pos[0]*freq+0.5, pos[1]*freq+0.5, pos[2]*freq+0.5, ctx.seed^12345))]
        }
        "FBM" => {
            let pos=v3i(0); let freq=fi(1,1.0); let oct=ii(2,4); let lac=fi(3,2.0);
            vec![PinValue::Float(ng_fbm(pos[0]*freq, pos[1]*freq, pos[2]*freq, oct, lac, 0.5, ctx.seed))]
        }
        "Voronoi" | "Worley" | "WorleyNoise" => {
            let pos=v3i(0); let freq=fi(1,1.0);
            vec![PinValue::Float(ng_worley(pos[0]*freq, pos[1]*freq, pos[2]*freq, ctx.seed))]
        }

        // Color
        "HSVtoRGB" => {
            let rgb = ng_hsv_to_rgb(fi(0,0.0), fi(1,1.0), fi(2,1.0));
            vec![PinValue::Color([rgb[0],rgb[1],rgb[2],1.0])]
        }
        "RGBtoHSV" => {
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let hsv=ng_rgb_to_hsv(c[0],c[1],c[2]);
            vec![PinValue::Float(hsv[0]),PinValue::Float(hsv[1]),PinValue::Float(hsv[2])]
        }
        "Gradient" => {
            let t=fi(0,0.0).clamp(0.0,1.0);
            let rgb=ng_hsv_to_rgb(t, 0.8, 1.0);
            vec![PinValue::Color([rgb[0],rgb[1],rgb[2],1.0])]
        }
        "ColorMix" => {
            let a=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let b=inputs.get(1).map(|v|v.as_vec4()).unwrap_or([1.0;4]);
            let t=fi(2,0.5);
            vec![PinValue::Color([ng_lerp(a[0],b[0],t),ng_lerp(a[1],b[1],t),ng_lerp(a[2],b[2],t),ng_lerp(a[3],b[3],t)])]
        }
        "ColorBrightness" => {
            let c=inputs.get(0).map(|v|v.as_vec4()).unwrap_or([0.0;4]);
            let f=fi(1,1.0);
            vec![PinValue::Color([c[0]*f, c[1]*f, c[2]*f, c[3]])]
        }

        // Force fields
        "Gravity" => {
            let center=v3i(0); let strength=fi(1,1.0);
            let diff=v3_sub(center, ctx.position);
            let dist=v3_len(diff).max(0.01);
            vec![PinValue::Vec3(v3_scale(v3_norm(diff), strength/(dist*dist)))]
        }
        "Vortex" => {
            let center=v3i(0); let strength=fi(1,1.0); let radius=fi(2,5.0);
            let diff=v3_sub(ctx.position, center);
            let dist=v3_len(diff).max(0.01);
            let tangent=v3_norm(v3_cross(diff, [0.0,1.0,0.0]));
            let falloff=(1.0-(dist/radius)).max(0.0);
            vec![PinValue::Vec3(v3_scale(tangent, strength*falloff))]
        }
        "Attractor" => {
            let center=v3i(0); let atype=ii(1,0); let scale=fi(2,1.0);
            let [px,py,pz]=ctx.position;
            let force = match atype {
                1 => { let (sig,rho,beta)=(10.0f32,28.0f32,8.0f32/3.0); [(sig*(py-px))*scale*0.01,(px*(rho-pz)-py)*scale*0.01,(px*py-beta*pz)*scale*0.01] }
                2 => { let (a,b,c)=(0.2f32,0.2f32,5.7f32); [(-py-pz)*scale*0.01,(px+a*py)*scale*0.01,(b+pz*(px-c))*scale*0.01] }
                _ => { let diff=v3_sub(center,ctx.position); let dist=v3_len(diff).max(0.01); v3_scale(v3_norm(diff),scale/dist) }
            };
            vec![PinValue::Vec3(force)]
        }
        "FlowField" => {
            let freq=fi(0,0.5); let scale=fi(1,1.0);
            let [px,py,pz]=ctx.position;
            let nx=ng_value_noise(px*freq, py*freq, pz*freq+0.0,  ctx.seed);
            let ny=ng_value_noise(px*freq, py*freq, pz*freq+10.0, ctx.seed);
            let nz=ng_value_noise(px*freq, py*freq, pz*freq+20.0, ctx.seed);
            vec![PinValue::Vec3([nx*scale, ny*scale, nz*scale])]
        }
        "FieldCompose" => {
            let a=v3i(0); let b=v3i(1); let w=fi(2,0.5);
            vec![PinValue::Vec3([ng_lerp(a[0],b[0],w),ng_lerp(a[1],b[1],w),ng_lerp(a[2],b[2],w)])]
        }

        // Logic
        "If"      => { let cond=inputs.get(0).map(|v|v.as_bool()).unwrap_or(false); vec![PinValue::Float(if cond {fi(1,1.0)} else {fi(2,0.0)})] }
        "Compare" => {
            let a=fi(0,0.0); let b=fi(1,0.0); let op=ii(2,0);
            let r=match op {0=>a==b,1=>a!=b,2=>a<b,3=>a<=b,4=>a>b,_=>a>=b};
            vec![PinValue::Bool(r)]
        }
        "And" => { let a=inputs.get(0).map(|v|v.as_bool()).unwrap_or(false); let b=inputs.get(1).map(|v|v.as_bool()).unwrap_or(false); vec![PinValue::Bool(a&&b)] }
        "Or"  => { let a=inputs.get(0).map(|v|v.as_bool()).unwrap_or(false); let b=inputs.get(1).map(|v|v.as_bool()).unwrap_or(false); vec![PinValue::Bool(a||b)] }
        "Not" => { let a=inputs.get(0).map(|v|v.as_bool()).unwrap_or(false); vec![PinValue::Bool(!a)] }

        // Output pass-through
        _ => inputs.to_vec(),
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GRAPH EVALUATOR
// ═══════════════════════════════════════════════════════════════════════════════

pub struct GraphEvalResult {
    pub node_outputs: HashMap<u32, Vec<PinValue>>,
    pub eval_order:   Vec<u32>,
    pub errors:       Vec<(u32, EvalError)>,
}

impl GraphEvalResult {
    pub fn get_output(&self, node_id: u32, pin_index: usize) -> Option<&PinValue> {
        self.node_outputs.get(&node_id)?.get(pin_index)
    }
    pub fn get_float(&self, node_id: u32, pin_index: usize) -> f32 {
        self.get_output(node_id, pin_index).map(|v| v.as_float()).unwrap_or(0.0)
    }
    pub fn get_vec3(&self, node_id: u32, pin_index: usize) -> [f32; 3] {
        self.get_output(node_id, pin_index).map(|v| v.as_vec3()).unwrap_or([0.0; 3])
    }
}

pub fn evaluate_graph(graph: &NodeGraph, ctx: &NodeEvalContext) -> GraphEvalResult {
    let order = graph.topological_order();
    let mut cache: HashMap<u32, Vec<PinValue>> = HashMap::new();
    let mut errors: Vec<(u32, EvalError)> = Vec::new();

    for &node_id in &order {
        let node = match graph.get_node(node_id) {
            Some(n) => n,
            None    => { errors.push((node_id, EvalError::NodeNotFound(node_id))); continue; }
        };

        let mut resolved: Vec<PinValue> = Vec::new();
        for input_pin in &node.inputs {
            let conn = graph.connections.iter().find(|c| c.to_pin == input_pin.id);
            let val = if let Some(c) = conn {
                let upstream = graph.get_node(c.from_node);
                let out_idx  = upstream.and_then(|n| n.outputs.iter().position(|p| p.id == c.from_pin));
                if let (Some(cached), Some(idx)) = (cache.get(&c.from_node), out_idx) {
                    cached.get(idx).cloned().unwrap_or_else(|| PinValue::default_for(input_pin.pin_type))
                } else {
                    PinValue::Float(input_pin.default_value)
                }
            } else {
                PinValue::Float(input_pin.default_value)
            };
            resolved.push(val);
        }

        let outputs = evaluate_node(node, &resolved, ctx);
        cache.insert(node_id, outputs);
    }

    GraphEvalResult { node_outputs: cache, eval_order: order, errors }
}

pub fn evaluate_to_node(graph: &NodeGraph, target: u32, ctx: &NodeEvalContext) -> Vec<PinValue> {
    evaluate_graph(graph, ctx).node_outputs.remove(&target).unwrap_or_default()
}

// ═══════════════════════════════════════════════════════════════════════════════
// TYPE COERCION API
// ═══════════════════════════════════════════════════════════════════════════════

pub fn can_connect(from: PinType, to: PinType) -> bool { from.can_convert_to(to) }

pub fn coerce(value: &PinValue, target: PinType) -> Option<PinValue> { value.coerce_to(target) }

// ═══════════════════════════════════════════════════════════════════════════════
// GRAPH SERIALIZATION
// ═══════════════════════════════════════════════════════════════════════════════

impl NodeGraph {
    pub fn to_json_full(&self) -> String {
        let mut s = String::with_capacity(4096);
        s.push_str("{\"name\":\"");
        s.push_str(&self.name.replace('"', "\\\""));
        s.push_str("\",\"nodes\":[");
        for (i, node) in self.nodes.iter().enumerate() {
            if i > 0 { s.push(','); }
            s.push_str(&format!("{{\"id\":{},\"name\":\"{}\",\"x\":{:.2},\"y\":{:.2},\"inputs\":[",
                node.id, node.name.replace('"', "\\\""), node.position.x, node.position.y));
            for (j, pin) in node.inputs.iter().enumerate() {
                if j > 0 { s.push(','); }
                s.push_str(&format!("{{\"id\":{},\"name\":\"{}\",\"type\":\"{}\",\"default\":{:.4}}}",
                    pin.id, pin.name.replace('"', "\\\""), pin.pin_type.label(), pin.default_value));
            }
            s.push_str("],\"outputs\":[");
            for (j, pin) in node.outputs.iter().enumerate() {
                if j > 0 { s.push(','); }
                s.push_str(&format!("{{\"id\":{},\"name\":\"{}\",\"type\":\"{}\"}}",
                    pin.id, pin.name.replace('"', "\\\""), pin.pin_type.label()));
            }
            s.push(']');
            if let Some(pv) = node.preview_value {
                s.push_str(&format!(",\"preview\":{:.4}", pv));
            }
            s.push('}');
        }
        s.push_str("],\"connections\":[");
        for (i, conn) in self.connections.iter().enumerate() {
            if i > 0 { s.push(','); }
            s.push_str(&format!("{{\"id\":{},\"fn\":{},\"fp\":{},\"tn\":{},\"tp\":{}}}",
                conn.id, conn.from_node, conn.from_pin, conn.to_node, conn.to_pin));
        }
        s.push_str("]}");
        s
    }

    pub fn from_json_str(s: &str) -> Result<NodeGraph, String> {
        let start = s.find("\"name\":\"").ok_or("missing name")?;
        let inner = &s[start + 8..];
        let end   = inner.find('"').ok_or("malformed name")?;
        Ok(NodeGraph::new(&inner[..end]))
    }

    pub fn to_lua(&self) -> String {
        let mut s = String::with_capacity(2048);
        s.push_str(&format!("-- NodeGraph: {}\nlocal graph = {{}}\n", self.name));
        for node in &self.nodes {
            s.push_str(&format!("graph[{}] = {{ name = \"{}\", x = {:.1}, y = {:.1} }}\n",
                node.id, node.name, node.position.x, node.position.y));
        }
        s.push_str("local connections = {\n");
        for conn in &self.connections {
            s.push_str(&format!("  {{ from={}, fp={}, to={}, tp={} }},\n",
                conn.from_node, conn.from_pin, conn.to_node, conn.to_pin));
        }
        s.push_str("}\n");
        s
    }

    pub fn to_glsl_snippet(&self) -> String {
        let mut s = String::with_capacity(2048);
        s.push_str(&format!("// NodeGraph: {}\n", self.name));
        for &nid in &self.topological_order() {
            if let Some(node) = self.get_node(nid) {
                let var = format!("n{}", nid);
                let line = match node.name.as_str() {
                    "Add"      => format!("float {} = n{}_a + n{}_b;", var, nid, nid),
                    "Multiply" => format!("float {} = n{}_a * n{}_b;", var, nid, nid),
                    "Sin"      => format!("float {} = sin(n{}_x);", var, nid),
                    "Cos"      => format!("float {} = cos(n{}_x);", var, nid),
                    "Sqrt"     => format!("float {} = sqrt(n{}_x);", var, nid),
                    "Lerp"     => format!("float {} = mix(n{}_a, n{}_b, n{}_t);", var, nid, nid, nid),
                    _          => format!("// {} (id={})", node.name, nid),
                };
                s.push_str(&line);
                s.push('\n');
            }
        }
        s
    }

    pub fn to_rust_snippet(&self) -> String {
        let mut s = String::with_capacity(2048);
        s.push_str(&format!("// NodeGraph: {}\n", self.name));
        for &nid in &self.topological_order() {
            if let Some(node) = self.get_node(nid) {
                let var = format!("n{}", nid);
                let line = match node.name.as_str() {
                    "Add"      => format!("let {} = n{}_a + n{}_b;", var, nid, nid),
                    "Multiply" => format!("let {} = n{}_a * n{}_b;", var, nid, nid),
                    "Sin"      => format!("let {} = n{}_x.sin();", var, nid),
                    "Cos"      => format!("let {} = n{}_x.cos();", var, nid),
                    "Sqrt"     => format!("let {} = n{}_x.max(0.0).sqrt();", var, nid),
                    _          => format!("// {} (id={})", node.name, nid),
                };
                s.push_str(&line);
                s.push('\n');
            }
        }
        s
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// LIVE PREVIEW HELPERS
// ═══════════════════════════════════════════════════════════════════════════════

pub fn format_pin_value_preview(val: &PinValue) -> String {
    match val {
        PinValue::Float(v) => format!("{:.3}", v),
        PinValue::Int(v)   => format!("{}", v),
        PinValue::Bool(v)  => format!("{}", v),
        PinValue::Vec2(v)  => format!("({:.2},{:.2})", v[0], v[1]),
        PinValue::Vec3(v)  => format!("({:.2},{:.2},{:.2})", v[0], v[1], v[2]),
        PinValue::Vec4(v)  => format!("({:.1},{:.1},{:.1},{:.1})", v[0], v[1], v[2], v[3]),
        PinValue::Color(v) => format!("rgba({:.2},{:.2},{:.2},{:.2})", v[0], v[1], v[2], v[3]),
    }
}

pub fn float_preview_bar(val: &PinValue) -> (f32, [f32; 3]) {
    let v = val.as_float();
    let ratio = (v * 0.5 + 0.5).clamp(0.0, 1.0);
    let col = if v < 0.0 { [0.3, 0.5, 0.9] } else { [0.3, 0.9, 0.4] };
    (ratio, col)
}

pub fn color_preview_rgba(val: &PinValue) -> [f32; 4] { val.as_vec4() }

/// Update all node preview_value fields by running the full graph evaluation.
pub fn update_node_previews(graph: &mut NodeGraph, ctx: &NodeEvalContext) {
    let result = evaluate_graph(graph, ctx);
    for node in &mut graph.nodes {
        if let Some(outputs) = result.node_outputs.get(&node.id) {
            node.preview_value = outputs.first().map(|v| v.as_float());
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// GRAPH VALIDATION
// ═══════════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct ValidationReport {
    pub cycles:       bool,
    pub type_errors:  Vec<(u32, u32, PinType, PinType)>,
    pub orphan_nodes: Vec<u32>,
}

impl ValidationReport {
    pub fn is_valid(&self) -> bool { !self.cycles && self.type_errors.is_empty() }

    pub fn summary(&self) -> String {
        if self.is_valid() {
            "Graph OK".to_string()
        } else {
            let mut msgs = Vec::new();
            if self.cycles { msgs.push("cycle detected".to_string()); }
            if !self.type_errors.is_empty() {
                msgs.push(format!("{} type error(s)", self.type_errors.len()));
            }
            msgs.join(", ")
        }
    }
}

pub fn validate_graph(graph: &NodeGraph) -> ValidationReport {
    let cycles = graph.has_cycles();
    let mut type_errors = Vec::new();
    for conn in &graph.connections {
        if let (Some(ft), Some(tt)) = (graph.pin_type(conn.from_pin), graph.pin_type(conn.to_pin)) {
            if !ft.can_convert_to(tt) {
                type_errors.push((conn.from_pin, conn.to_pin, ft, tt));
            }
        }
    }
    let order = graph.topological_order();
    let orphan_nodes: Vec<u32> = graph.nodes.iter()
        .filter(|n| n.category == NodeCategory::Output && !order.contains(&n.id))
        .map(|n| n.id)
        .collect();
    ValidationReport { cycles, type_errors, orphan_nodes }
}

// ═══════════════════════════════════════════════════════════════════════════════
// ADDITIONAL FACTORY NODES
// ═══════════════════════════════════════════════════════════════════════════════

impl NodeFactory {
    pub fn random(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Random", NodeCategory::Source, x, y)
            .with_output(pin_start, "val", PinType::Float)
    }
    pub fn slider(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        let mut n = GraphNode::new(id, "Slider", NodeCategory::Source, x, y)
            .with_output(pin_start, "val", PinType::Float);
        n.preview_value = Some(0.5);
        n
    }
    pub fn mouse_pos(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "MousePos", NodeCategory::Source, x, y)
            .with_output(pin_start, "pos", PinType::Vec2)
    }
    pub fn tan(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Tan", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start+1, "tan", PinType::Float)
    }
    pub fn atan2(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Atan2", NodeCategory::Math, x, y)
            .with_input(pin_start, "y", PinType::Float)
            .with_input(pin_start+1, "x", PinType::Float)
            .with_output(pin_start+2, "angle", PinType::Float)
    }
    pub fn floor_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Floor", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start+1, "floor", PinType::Float)
    }
    pub fn ceil_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Ceil", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start+1, "ceil", PinType::Float)
    }
    pub fn sign_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Sign", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start+1, "sign", PinType::Float)
    }
    pub fn min_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Min", NodeCategory::Math, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start+1, "b", PinType::Float)
            .with_output(pin_start+2, "min", PinType::Float)
    }
    pub fn max_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Max", NodeCategory::Math, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start+1, "b", PinType::Float)
            .with_output(pin_start+2, "max", PinType::Float)
    }
    pub fn mod_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Mod", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_input(pin_start+1, "m", PinType::Float)
            .with_output(pin_start+2, "result", PinType::Float)
    }
    pub fn log_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Log", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start+1, "ln", PinType::Float)
    }
    pub fn exp_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Exp", NodeCategory::Math, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_output(pin_start+1, "e^x", PinType::Float)
    }
    pub fn step_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Step", NodeCategory::Math, x, y)
            .with_input(pin_start, "edge", PinType::Float)
            .with_input(pin_start+1, "x", PinType::Float)
            .with_output(pin_start+2, "result", PinType::Float)
    }
    pub fn make_vec2(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "MakeVec2", NodeCategory::Vector, x, y)
            .with_input(pin_start, "x", PinType::Float)
            .with_input(pin_start+1, "y", PinType::Float)
            .with_output(pin_start+2, "vec", PinType::Vec2)
    }
    pub fn split_vec2(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "SplitVec2", NodeCategory::Vector, x, y)
            .with_input(pin_start, "vec", PinType::Vec2)
            .with_output(pin_start+1, "x", PinType::Float)
            .with_output(pin_start+2, "y", PinType::Float)
    }
    pub fn cross(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Cross", NodeCategory::Vector, x, y)
            .with_input(pin_start, "a", PinType::Vec3)
            .with_input(pin_start+1, "b", PinType::Vec3)
            .with_output(pin_start+2, "cross", PinType::Vec3)
    }
    pub fn reflect_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Reflect", NodeCategory::Vector, x, y)
            .with_input(pin_start, "v", PinType::Vec3)
            .with_input(pin_start+1, "n", PinType::Vec3)
            .with_output(pin_start+2, "reflected", PinType::Vec3)
    }
    pub fn rotate2d(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Rotate2D", NodeCategory::Vector, x, y)
            .with_input(pin_start, "v", PinType::Vec2)
            .with_input(pin_start+1, "angle", PinType::Float)
            .with_output(pin_start+2, "rotated", PinType::Vec2)
    }
    pub fn distance_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Distance", NodeCategory::Vector, x, y)
            .with_input(pin_start, "a", PinType::Vec3)
            .with_input(pin_start+1, "b", PinType::Vec3)
            .with_output(pin_start+2, "dist", PinType::Float)
    }
    pub fn value_noise(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "ValueNoise", NodeCategory::Noise, x, y)
            .with_input(pin_start, "pos", PinType::Vec3)
            .with_input(pin_start+1, "freq", PinType::Float)
            .with_output(pin_start+2, "noise", PinType::Float)
    }
    pub fn simplex(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Simplex", NodeCategory::Noise, x, y)
            .with_input(pin_start, "pos", PinType::Vec3)
            .with_input(pin_start+1, "freq", PinType::Float)
            .with_output(pin_start+2, "noise", PinType::Float)
    }
    pub fn worley(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Worley", NodeCategory::Noise, x, y)
            .with_input(pin_start, "pos", PinType::Vec3)
            .with_input(pin_start+1, "freq", PinType::Float)
            .with_output(pin_start+2, "dist", PinType::Float)
    }
    pub fn rgb_to_hsv(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "RGBtoHSV", NodeCategory::Color, x, y)
            .with_input(pin_start, "rgb", PinType::Color)
            .with_output(pin_start+1, "h", PinType::Float)
            .with_output(pin_start+2, "s", PinType::Float)
            .with_output(pin_start+3, "v", PinType::Float)
    }
    pub fn color_mix(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "ColorMix", NodeCategory::Color, x, y)
            .with_input(pin_start, "a", PinType::Color)
            .with_input(pin_start+1, "b", PinType::Color)
            .with_input(pin_start+2, "t", PinType::Float)
            .with_output(pin_start+3, "color", PinType::Color)
    }
    pub fn color_brightness(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "ColorBrightness", NodeCategory::Color, x, y)
            .with_input(pin_start, "color", PinType::Color)
            .with_input(pin_start+1, "factor", PinType::Float)
            .with_output(pin_start+2, "out", PinType::Color)
    }
    pub fn flow_field(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "FlowField", NodeCategory::ForceField, x, y)
            .with_input(pin_start, "freq", PinType::Float)
            .with_input(pin_start+1, "scale", PinType::Float)
            .with_output(pin_start+2, "force", PinType::Vec3)
    }
    pub fn field_compose(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "FieldCompose", NodeCategory::ForceField, x, y)
            .with_input(pin_start, "a", PinType::Vec3)
            .with_input(pin_start+1, "b", PinType::Vec3)
            .with_input(pin_start+2, "weight", PinType::Float)
            .with_output(pin_start+3, "force", PinType::Vec3)
    }
    pub fn if_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "If", NodeCategory::Logic, x, y)
            .with_input(pin_start, "cond", PinType::Bool)
            .with_input(pin_start+1, "true_val", PinType::Float)
            .with_input(pin_start+2, "false_val", PinType::Float)
            .with_output(pin_start+3, "result", PinType::Float)
    }
    pub fn compare(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Compare", NodeCategory::Logic, x, y)
            .with_input(pin_start, "a", PinType::Float)
            .with_input(pin_start+1, "b", PinType::Float)
            .with_input(pin_start+2, "op", PinType::Int)
            .with_output(pin_start+3, "result", PinType::Bool)
    }
    pub fn and_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "And", NodeCategory::Logic, x, y)
            .with_input(pin_start, "a", PinType::Bool)
            .with_input(pin_start+1, "b", PinType::Bool)
            .with_output(pin_start+2, "result", PinType::Bool)
    }
    pub fn or_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Or", NodeCategory::Logic, x, y)
            .with_input(pin_start, "a", PinType::Bool)
            .with_input(pin_start+1, "b", PinType::Bool)
            .with_output(pin_start+2, "result", PinType::Bool)
    }
    pub fn not_node(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Not", NodeCategory::Logic, x, y)
            .with_input(pin_start, "a", PinType::Bool)
            .with_output(pin_start+1, "result", PinType::Bool)
    }
    pub fn output_velocity(id: u32, pin_start: u32, x: f32, y: f32) -> GraphNode {
        GraphNode::new(id, "Out:Velocity", NodeCategory::Output, x, y)
            .with_input(pin_start, "vel", PinType::Vec3)
    }

    pub fn full_catalog() -> Vec<(&'static str, NodeCategory, fn(u32,u32,f32,f32) -> GraphNode)> {
        let mut cat = Self::catalog();
        cat.extend(vec![
            ("Random",          NodeCategory::Source,     Self::random          as fn(u32,u32,f32,f32)->GraphNode),
            ("Slider",          NodeCategory::Source,     Self::slider),
            ("MousePos",        NodeCategory::Source,     Self::mouse_pos),
            ("Tan",             NodeCategory::Math,       Self::tan),
            ("Atan2",           NodeCategory::Math,       Self::atan2),
            ("Floor",           NodeCategory::Math,       Self::floor_node),
            ("Ceil",            NodeCategory::Math,       Self::ceil_node),
            ("Sign",            NodeCategory::Math,       Self::sign_node),
            ("Min",             NodeCategory::Math,       Self::min_node),
            ("Max",             NodeCategory::Math,       Self::max_node),
            ("Mod",             NodeCategory::Math,       Self::mod_node),
            ("Log",             NodeCategory::Math,       Self::log_node),
            ("Exp",             NodeCategory::Math,       Self::exp_node),
            ("Step",            NodeCategory::Math,       Self::step_node),
            ("MakeVec2",        NodeCategory::Vector,     Self::make_vec2),
            ("SplitVec2",       NodeCategory::Vector,     Self::split_vec2),
            ("Cross",           NodeCategory::Vector,     Self::cross),
            ("Reflect",         NodeCategory::Vector,     Self::reflect_node),
            ("Rotate2D",        NodeCategory::Vector,     Self::rotate2d),
            ("Distance",        NodeCategory::Vector,     Self::distance_node),
            ("ValueNoise",      NodeCategory::Noise,      Self::value_noise),
            ("Simplex",         NodeCategory::Noise,      Self::simplex),
            ("Worley",          NodeCategory::Noise,      Self::worley),
            ("RGBtoHSV",        NodeCategory::Color,      Self::rgb_to_hsv),
            ("ColorMix",        NodeCategory::Color,      Self::color_mix),
            ("ColorBrightness", NodeCategory::Color,      Self::color_brightness),
            ("FlowField",       NodeCategory::ForceField, Self::flow_field),
            ("FieldCompose",    NodeCategory::ForceField, Self::field_compose),
            ("If",              NodeCategory::Logic,      Self::if_node),
            ("Compare",         NodeCategory::Logic,      Self::compare),
            ("And",             NodeCategory::Logic,      Self::and_node),
            ("Or",              NodeCategory::Logic,      Self::or_node),
            ("Not",             NodeCategory::Logic,      Self::not_node),
            ("Out:Velocity",    NodeCategory::Output,     Self::output_velocity),
        ]);
        cat
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// BATCH EVALUATION
// ═══════════════════════════════════════════════════════════════════════════════

pub fn evaluate_over_grid(
    graph: &NodeGraph,
    output_node_id: u32,
    grid_w: usize,
    grid_h: usize,
    time: f32,
) -> Vec<f32> {
    let mut results = Vec::with_capacity(grid_w * grid_h);
    for gy in 0..grid_h {
        for gx in 0..grid_w {
            let px = gx as f32 / (grid_w as f32 - 1.0).max(1.0);
            let py = gy as f32 / (grid_h as f32 - 1.0).max(1.0);
            let ctx = NodeEvalContext { time, position: [px,py,0.0], seed: 0, custom_vars: HashMap::new() };
            let vals = evaluate_to_node(graph, output_node_id, &ctx);
            results.push(vals.first().map(|v| v.as_float()).unwrap_or(0.0));
        }
    }
    results
}

pub fn evaluate_over_time(
    graph: &NodeGraph,
    output_node_id: u32,
    t_start: f32,
    t_end: f32,
    steps: usize,
) -> Vec<f32> {
    let mut results = Vec::with_capacity(steps);
    for i in 0..steps {
        let t = t_start + (t_end - t_start) * i as f32 / (steps as f32 - 1.0).max(1.0);
        let ctx = NodeEvalContext::new(t);
        let vals = evaluate_to_node(graph, output_node_id, &ctx);
        results.push(vals.first().map(|v| v.as_float()).unwrap_or(0.0));
    }
    results
}

// ═══════════════════════════════════════════════════════════════════════════════
// ADDITIONAL EXAMPLE GRAPHS
// ═══════════════════════════════════════════════════════════════════════════════

impl NodeGraph {
    pub fn example_sine_wave() -> Self {
        let mut g = NodeGraph::new("SineWave");
        let _t   = g.add_node(NodeFactory::time,            0.0, 0.0);
        let _sin = g.add_node(NodeFactory::sin,             12.0, 0.0);
        let _out = g.add_node(NodeFactory::output_emission, 24.0, 0.0);
        g
    }

    pub fn example_noise_color() -> Self {
        let mut g = NodeGraph::new("NoiseColor");
        let _pos  = g.add_node(NodeFactory::position,     0.0, 0.0);
        let _fbm  = g.add_node(NodeFactory::fbm,          12.0, 0.0);
        let _grad = g.add_node(NodeFactory::gradient,     24.0, 0.0);
        let _out  = g.add_node(NodeFactory::output_color, 36.0, 0.0);
        g
    }

    pub fn example_lorenz() -> Self {
        let mut g = NodeGraph::new("Lorenz");
        let _pos  = g.add_node(NodeFactory::position,         0.0, 0.0);
        let _attr = g.add_node(NodeFactory::attractor_field, 12.0, 0.0);
        let _out  = g.add_node(NodeFactory::output_force,    24.0, 0.0);
        g
    }

    pub fn example_vortex_flow() -> Self {
        let mut g = NodeGraph::new("VortexFlow");
        let _pos     = g.add_node(NodeFactory::position,      0.0,  0.0);
        let _vortex  = g.add_node(NodeFactory::vortex_field, 12.0,  0.0);
        let _flow    = g.add_node(NodeFactory::flow_field,   12.0, -8.0);
        let _compose = g.add_node(NodeFactory::field_compose, 24.0, 0.0);
        let _out     = g.add_node(NodeFactory::output_force,  36.0, 0.0);
        g
    }
}
